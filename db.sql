create table "user"
(
    id              uuid                                   not null
        primary key,
    email           varchar                                not null,
    name            varchar                                not null,
    created_at      timestamp with time zone default now() not null,
    registered_at   timestamp with time zone,
    discord_webhook varchar
);

alter table "user"
    owner to postgres;

create table magic_link
(
    id         uuid                                   not null
        primary key,
    email      varchar                                not null,
    created_at timestamp with time zone default now() not null,
    state      varchar                                not null,
    token      uuid                                   not null
);

alter table magic_link
    owner to postgres;

create table website
(
    url                    varchar                                            not null,
    keyword                varchar                                            not null,
    tags                   varchar                                            not null,
    user_id                uuid                                               not null
        references "user",
    useragent              varchar,
    created_at             timestamp with time zone default now()             not null,
    id                     uuid                     default gen_random_uuid() not null
        primary key,
    domain_expire_at       timestamp with time zone,
    last_domain_checked_at timestamp with time zone,
    ssl_expire_at          timestamp with time zone,
    is_deleted             boolean                  default false             not null,
    last_ssl_checked_at    timestamp with time zone,
    is_paused              boolean                  default false             not null
);

alter table website
    owner to postgres;



create table website_state
(
    created_at timestamp with time zone default now()             not null,
    state      text                                               not null,
    id         uuid                     default gen_random_uuid() not null
        primary key,
    website_id uuid                                               not null
        references website,
    duration   bigint                   default 0                 not null
);

alter table website_state
    owner to postgres;

