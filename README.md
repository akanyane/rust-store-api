# Rust Store API

API de loja online escrita em Rust com Axum: produtos, variantes, carrinho, autenticação e pedidos com checkout transacional.

## Stack

- Rust (edition 2024)
- [Axum](https://github.com/tokio-rs/axum) 0.8 sobre Tokio
- [SurrealDB](https://surrealdb.com) 3.3 embarcado (`surrealkv`), sem servidor de banco externo
- argon2 (hash de senha) e SHA-256 (hash dos tokens de sessão)

## Como rodar

```bash
cp .env.example .env
cargo run
```

O banco é criado em `data/rust-store` (ignorado pelo git) e o schema (`schema.surql`) é aplicado a cada partida. O servidor sobe em `http://localhost:3000` por padrão.

### Variáveis de ambiente

| Variável | Obrigatória | Descrição |
|---|---|---|
| `PORT` | não | Porta HTTP. Padrão `3000`. |
| `ADMIN_EMAIL` | não* | E-mail do admin criado na partida. |
| `ADMIN_PASSWORD` | não* | Senha do admin criado na partida. |

\* Defina `ADMIN_EMAIL` e `ADMIN_PASSWORD` juntas, ou nenhuma das duas. Valor vazio conta como ausente e a loja sobe sem admin. A criação é idempotente: não troca a senha nem promove um usuário comum já existente.

## Rotas

Acesso: **pública**, **cliente** (Bearer token de um cliente) ou **admin** (Bearer token de um admin).

| Método | Rota | Acesso | Descrição |
|---|---|---|---|
| GET | `/health` | pública | Health check |
| POST | `/auth/sign-up` | pública | Cadastro de cliente |
| POST | `/auth/sign-in` | pública | Login; devolve `session_token` e `refresh_token` |
| POST | `/auth/sign-out` | pública | Encerra a sessão a partir do `refresh_token` |
| GET | `/products` | pública | Lista produtos |
| POST | `/products` | admin | Cria produto |
| GET | `/products/{id}` | pública | Detalha produto |
| GET | `/products/{id}/variants` | pública | Lista variantes do produto |
| POST | `/products/{id}/variants` | admin | Cria variante |
| GET, PUT, DELETE | `/me` | cliente | Consulta, atualiza e remove o próprio cadastro |
| GET | `/cart` | cliente | Consulta o carrinho |
| POST | `/cart/items` | cliente | Adiciona variante ao carrinho |
| PUT, DELETE | `/cart/items/{variant_id}` | cliente | Altera quantidade ou remove item |
| GET | `/orders` | cliente | Lista os pedidos do cliente |
| POST | `/orders` | cliente | Checkout do carrinho |
| GET | `/orders/{id}` | cliente | Detalha um pedido do cliente |

## Autenticação

Rotas protegidas esperam o cabeçalho `Authorization: Bearer <session_token>`.

- A sessão dura 1 hora e o refresh token 15 dias.
- Senhas são guardadas com argon2; tokens, apenas como hash SHA-256.
- O cliente é sempre identificado pelo token, nunca por um id na URL.

## Checkout

`POST /orders` roda em uma única transação: revalida que variante e produto estão ativos, baixa o estoque (`UPDATE ... WHERE stock >= quantity`), cria o pedido `pending` com um retrato de nome, SKU e preço de cada item, e esvazia o carrinho. Se qualquer passo falhar, nada é gravado.

## Estrutura

```
src/
├── main.rs         # rotas e inicialização
├── db.rs           # conexão e schema
├── state.rs        # AppState
├── error.rs        # AppError e respostas HTTP
├── executor.rs     # abstração sobre conexão e transação
├── extractors.rs   # AuthCustomer e AuthAdmin
├── handlers/       # camada HTTP
├── services/       # regras de negócio
├── repositories/   # acesso ao SurrealDB
└── models/         # tipos de entrada e saída
schema.surql        # definição das tabelas
```

## Limitações conhecidas

Pagamento, cancelamento de pedido com devolução de estoque, refresh de token, limpeza de sessões expiradas e validação de entrada ainda não foram implementados.
