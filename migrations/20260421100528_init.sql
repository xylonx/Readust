-- Add migration script here

CREATE TABLE public.users (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    email TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    CONSTRAINT users_pkey PRIMARY KEY (id)
);

CREATE UNIQUE INDEX idx_users_email ON public.users (email);

CREATE TABLE public.tokens (
    id UUID NOT NULL,
    user_id UUID NOT NULL,
    refresh_token UUID NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    CONSTRAINT tokens_pkey PRIMARY KEY (id)
);

CREATE INDEX idx_tokens_user_id ON public.tokens (user_id);
CREATE UNIQUE INDEX idx_tokens_refresh_token ON public.tokens (refresh_token);

CREATE TABLE public.books (
    user_id UUID NOT NULL,
    book_hash TEXT NOT NULL,
    meta_hash TEXT NULL,
    format TEXT NULL,
    title TEXT NULL,
    source_title TEXT NULL,
    author TEXT NULL,
    "group" TEXT NULL,
    tags TEXT[] NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    uploaded_at TIMESTAMP WITH TIME ZONE NULL,
    progress INTEGER[] NOT NULL DEFAULT '{}',
    reading_status TEXT NULL,
    group_id TEXT NULL,
    group_name TEXT NULL,
    metadata JSON NULL,
    CONSTRAINT books_pkey PRIMARY KEY (user_id, book_hash)
);

CREATE INDEX idx_books_user_id_meta_hash ON public.books (user_id, meta_hash);

CREATE TABLE public.book_configs (
    user_id UUID NOT NULL,
    book_hash TEXT NOT NULL,
    meta_hash TEXT NULL,
    location TEXT NULL,
    xpointer TEXT NULL,
    progress JSONB NULL,
    rsvp_position TEXT NULL,
    search_config JSONB NULL,
    view_settings JSONB NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    CONSTRAINT book_configs_pkey PRIMARY KEY (user_id, book_hash)
);

CREATE INDEX idx_book_configs_user_id_meta_hash ON public.book_configs (user_id, meta_hash);

CREATE TABLE public.book_notes (
    user_id UUID NOT NULL,
    book_hash TEXT NOT NULL,
    meta_hash TEXT NULL,
    id TEXT NOT NULL,
    type TEXT NULL,
    cfi TEXT NULL,
    xpointer0 TEXT NULL,
    xpointer1 TEXT NULL,
    text TEXT NULL,
    style TEXT NULL,
    color TEXT NULL,
    note TEXT NULL,
    page INTEGER NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    CONSTRAINT book_notes_pkey PRIMARY KEY (user_id, book_hash, id)
);

CREATE INDEX idx_book_notes_user_id_meta_hash ON public.book_notes (user_id, meta_hash);

CREATE TABLE public.files (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    book_hash TEXT NULL,
    file_key TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    deleted_at TIMESTAMP WITH TIME ZONE NULL,
    CONSTRAINT files_pkey PRIMARY KEY (id)
);

CREATE INDEX idx_files_file_key ON public.files (file_key);
CREATE INDEX idx_files_valid_user_id_book_hash ON public.files (user_id, book_hash) WHERE deleted_at IS NULL;
