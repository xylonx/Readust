-- Migrate from https://github.com/readest/readest/blob/main/docker/volumes/db/migrations/*
--
-- ─────────────────────────────────────────────────────────────────────────
-- replicas: polymorphic per-user CRDT-backed metadata.
--   kind            — server-allowlisted: 'dictionary' in PR 1; future kinds
--                     require a server release that updates the CHECK below.
--   fields_jsonb    — per-field LWW envelope: {<field>: {v, t: <Hlc>, s}}
--                     PR-validated 64 KiB / 64-field caps server-side.
--   manifest_jsonb  — committed last after binary upload completes.
--                     null = "binaries pending"; row not yet downloadable.
--   deleted_at_ts   — remove-wins tombstone HLC. A field write does NOT
--                     revive a tombstoned row.
--   reincarnation   — explicit re-import token; swaps row to alive under a
--                     new logical identity.
--   updated_at_ts   — max(field HLCs, deleted_at_ts, row-level operation
--                     HLCs such as manifest commits). Used as the pull cursor.
--   schema_version  — per-kind schema bump; server enforces bounds.
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE public.replicas (
  user_id uuid NOT NULL,
  kind text NOT NULL,
  replica_id text NOT NULL,
  fields_jsonb jsonb NOT NULL DEFAULT '{}'::jsonb,
  manifest_jsonb jsonb NULL,
  deleted_at_ts text NULL,
  reincarnation text NULL,
  updated_at_ts text NOT NULL,
  schema_version integer NOT NULL DEFAULT 1,
  created_at timestamp with time zone NOT NULL DEFAULT now(),
  modified_at timestamp with time zone NOT NULL DEFAULT now(),
  CONSTRAINT replicas_pkey PRIMARY KEY (user_id, kind, replica_id)
);

CREATE INDEX IF NOT EXISTS idx_replicas_pull_cursor
  ON public.replicas (user_id, kind, updated_at_ts);

-- ─────────────────────────────────────────────────────────────────────────
-- HLC max helper. NULLs lose. Plain text comparison since the HLC packing
-- format makes lexicographic order match temporal order.
-- ─────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION public.hlc_max(a text, b text)
RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$
  SELECT CASE
    WHEN a IS NULL THEN b
    WHEN b IS NULL THEN a
    WHEN a >= b THEN a
    ELSE b
  END;
$$;

-- ─────────────────────────────────────────────────────────────────────────
-- Field-level LWW merge for fields_jsonb. Per-key: keep the envelope with
-- the larger envelope.t (HLC string). Tie on HLC: deviceId (envelope.s)
-- lex-order tiebreak. Preserves keys present on either side.
-- ─────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION public.crdt_merge_fields(local_fields jsonb, remote_fields jsonb)
RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE
AS $$
DECLARE
  result jsonb := COALESCE(local_fields, '{}'::jsonb);
  k text;
  l_env jsonb;
  r_env jsonb;
  l_t text;
  r_t text;
  l_s text;
  r_s text;
BEGIN
  IF remote_fields IS NULL THEN
    RETURN result;
  END IF;
  FOR k IN SELECT jsonb_object_keys(remote_fields) LOOP
    r_env := remote_fields -> k;
    l_env := result -> k;
    IF l_env IS NULL THEN
      result := jsonb_set(result, ARRAY[k], r_env, true);
    ELSE
      l_t := l_env ->> 't';
      r_t := r_env ->> 't';
      IF r_t > l_t THEN
        result := jsonb_set(result, ARRAY[k], r_env, true);
      ELSIF r_t = l_t THEN
        l_s := COALESCE(l_env ->> 's', '');
        r_s := COALESCE(r_env ->> 's', '');
        IF r_s > l_s THEN
          result := jsonb_set(result, ARRAY[k], r_env, true);
        END IF;
      END IF;
    END IF;
  END LOOP;
  RETURN result;
END;
$$;

-- ─────────────────────────────────────────────────────────────────────────
-- Content updated_at_ts = max over field HLCs and tombstone HLC.
-- ─────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION public.crdt_compute_updated_at(fields jsonb, deleted_at text)
RETURNS text
LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE
AS $$
DECLARE
  result text := COALESCE(deleted_at, '0000000000000-00000000-');
  k text;
  env jsonb;
  t text;
BEGIN
  IF fields IS NULL THEN
    RETURN result;
  END IF;
  FOR k IN SELECT jsonb_object_keys(fields) LOOP
    env := fields -> k;
    t := env ->> 't';
    IF t IS NOT NULL AND t > result THEN
      result := t;
    END IF;
  END LOOP;
  RETURN result;
END;
$$;

-- ─────────────────────────────────────────────────────────────────────────
-- Full row merge. Used in:
--   INSERT INTO replicas (...) VALUES (...)
--   ON CONFLICT (user_id, kind, replica_id) DO UPDATE SET
--     fields_jsonb   = crdt_merge_fields(replicas.fields_jsonb, EXCLUDED.fields_jsonb),
--     deleted_at_ts  = hlc_max(replicas.deleted_at_ts, EXCLUDED.deleted_at_ts),
--     reincarnation  = CASE WHEN replicas.reincarnation = EXCLUDED.reincarnation
--                           THEN replicas.reincarnation
--                           WHEN EXCLUDED.updated_at_ts > replicas.updated_at_ts
--                           THEN EXCLUDED.reincarnation
--                           ELSE replicas.reincarnation END,
--     manifest_jsonb = CASE WHEN EXCLUDED.updated_at_ts > replicas.updated_at_ts
--                           THEN EXCLUDED.manifest_jsonb
--                           ELSE replicas.manifest_jsonb END,
--     schema_version = GREATEST(replicas.schema_version, EXCLUDED.schema_version),
--     updated_at_ts  = crdt_compute_updated_at(
--                        crdt_merge_fields(replicas.fields_jsonb, EXCLUDED.fields_jsonb),
--                        hlc_max(replicas.deleted_at_ts, EXCLUDED.deleted_at_ts)
--                      ),
--     modified_at    = now()
--
-- Or via the wrapper below for shorter call sites.
-- ─────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION public.crdt_merge_replica(
  p_user_id uuid,
  p_kind text,
  p_replica_id text,
  p_fields_jsonb jsonb,
  p_manifest_jsonb jsonb,
  p_deleted_at_ts text,
  p_reincarnation text,
  p_updated_at_ts text,
  p_schema_version integer
) RETURNS public.replicas
LANGUAGE plpgsql
AS $$
DECLARE
  result public.replicas;
BEGIN
  INSERT INTO public.replicas AS r (
    user_id, kind, replica_id,
    fields_jsonb, manifest_jsonb, deleted_at_ts,
    reincarnation, updated_at_ts, schema_version
  ) VALUES (
    p_user_id, p_kind, p_replica_id,
    COALESCE(p_fields_jsonb, '{}'::jsonb),
    p_manifest_jsonb, p_deleted_at_ts,
    p_reincarnation, p_updated_at_ts, p_schema_version
  )
  ON CONFLICT (user_id, kind, replica_id) DO UPDATE SET
    fields_jsonb   = public.crdt_merge_fields(r.fields_jsonb, EXCLUDED.fields_jsonb),
    deleted_at_ts  = public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts),
    reincarnation  = CASE
                       WHEN r.reincarnation IS NOT DISTINCT FROM EXCLUDED.reincarnation
                         THEN r.reincarnation
                       WHEN EXCLUDED.updated_at_ts > r.updated_at_ts
                         THEN EXCLUDED.reincarnation
                       ELSE r.reincarnation
                     END,
    manifest_jsonb = CASE
                       WHEN EXCLUDED.updated_at_ts > r.updated_at_ts
                         THEN EXCLUDED.manifest_jsonb
                       ELSE r.manifest_jsonb
                     END,
    schema_version = GREATEST(r.schema_version, EXCLUDED.schema_version),
    updated_at_ts  = public.crdt_compute_updated_at(
                       public.crdt_merge_fields(r.fields_jsonb, EXCLUDED.fields_jsonb),
                       public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                     ),
    modified_at    = now()
  RETURNING * INTO result;
  RETURN result;
END;
$$;

CREATE OR REPLACE FUNCTION public.crdt_merge_replica(
  p_user_id uuid,
  p_kind text,
  p_replica_id text,
  p_fields_jsonb jsonb,
  p_manifest_jsonb jsonb,
  p_deleted_at_ts text,
  p_reincarnation text,
  p_updated_at_ts text,
  p_schema_version integer
) RETURNS public.replicas
LANGUAGE plpgsql
AS $$
DECLARE
  result public.replicas;
BEGIN
  INSERT INTO public.replicas AS r (
    user_id, kind, replica_id,
    fields_jsonb, manifest_jsonb, deleted_at_ts,
    reincarnation, updated_at_ts, schema_version
  ) VALUES (
    p_user_id, p_kind, p_replica_id,
    COALESCE(p_fields_jsonb, '{}'::jsonb),
    p_manifest_jsonb, p_deleted_at_ts,
    p_reincarnation, p_updated_at_ts, p_schema_version
  )
  ON CONFLICT (user_id, kind, replica_id) DO UPDATE SET
    fields_jsonb   = public.crdt_merge_fields(r.fields_jsonb, EXCLUDED.fields_jsonb),
    deleted_at_ts  = public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts),
    reincarnation  = CASE
                       WHEN r.reincarnation IS NULL AND EXCLUDED.reincarnation IS NULL
                         THEN NULL
                       WHEN r.reincarnation IS NOT NULL AND EXCLUDED.reincarnation IS NULL
                         THEN CASE
                                WHEN public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts) IS NULL
                                  OR r.updated_at_ts > public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                                  THEN r.reincarnation
                                ELSE NULL
                              END
                       WHEN r.reincarnation IS NULL AND EXCLUDED.reincarnation IS NOT NULL
                         THEN CASE
                                WHEN public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts) IS NULL
                                  OR EXCLUDED.updated_at_ts > public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                                  THEN EXCLUDED.reincarnation
                                ELSE NULL
                              END
                       WHEN EXCLUDED.updated_at_ts > r.updated_at_ts
                         THEN CASE
                                WHEN public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts) IS NULL
                                  OR EXCLUDED.updated_at_ts > public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                                  THEN EXCLUDED.reincarnation
                                ELSE NULL
                              END
                       ELSE CASE
                              WHEN public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts) IS NULL
                                OR r.updated_at_ts > public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                                THEN r.reincarnation
                              ELSE NULL
                            END
                     END,
    manifest_jsonb = CASE
                       WHEN EXCLUDED.manifest_jsonb IS NULL
                         THEN r.manifest_jsonb
                       WHEN r.manifest_jsonb IS NULL
                         THEN EXCLUDED.manifest_jsonb
                       WHEN EXCLUDED.updated_at_ts > r.updated_at_ts
                         THEN EXCLUDED.manifest_jsonb
                       ELSE r.manifest_jsonb
                     END,
    schema_version = GREATEST(r.schema_version, EXCLUDED.schema_version),
    updated_at_ts  = public.hlc_max(
                       public.hlc_max(r.updated_at_ts, EXCLUDED.updated_at_ts),
                       public.crdt_compute_updated_at(
                         public.crdt_merge_fields(r.fields_jsonb, EXCLUDED.fields_jsonb),
                         public.hlc_max(r.deleted_at_ts, EXCLUDED.deleted_at_ts)
                       )
                     ),
    modified_at    = now()
  RETURNING * INTO result;
  RETURN result;
END;
$$;
