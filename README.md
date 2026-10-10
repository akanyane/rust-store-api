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
| `ADMIN_USERNAME` | não* | Username do admin criado na partida. |
| `ADMIN_PASSWORD` | não* | Senha do admin criado na partida. |

\* Defina `ADMIN_USERNAME` e `ADMIN_PASSWORD` juntas, ou nenhuma das duas. Valor vazio conta como ausente e a loja sobe sem admin. A criação é idempotente: não troca a senha nem promove um usuário comum já existente.

## Rotas

Acesso: **pública**, **cliente** (Bearer token de um cliente) ou **admin** (Bearer token de um admin).

| Método | Rota | Acesso | Descrição |
|---|---|---|---|
| GET | `/health` | pública | Health check |
| POST | `/auth/sign-up` | pública | Cadastro de cliente (`username`, `password`, nomes e nascimento) |
| POST | `/auth/sign-in` | pública | Login; devolve `session_token` e `refresh_token` |
| POST | `/auth/sign-out` | pública | Encerra a sessão a partir do `refresh_token` |
| POST | `/auth/refresh` | pública | Troca um `refresh_token` por um par novo de tokens |
| GET | `/products` | pública | Lista produtos |
| POST | `/products` | admin | Cria produto |
| GET | `/products/{id}` | pública | Detalha produto |
| GET | `/products/{id}/variants` | pública | Lista variantes do produto |
| POST | `/products/{id}/variants` | admin | Cria variante |
| PUT | `/products/{id}` | admin | Substitui nome, descrição e `active` do produto |
| DELETE | `/products/{id}` | admin | Desativa o produto (exclusão lógica) |
| PUT | `/products/{id}/variants/{variant_id}` | admin | Substitui nome, SKU, preço, estoque e `active` da variante |
| DELETE | `/products/{id}/variants/{variant_id}` | admin | Desativa a variante (exclusão lógica) |
| GET | `/admin/products` | admin | Lista produtos ativos e inativos (`?active=`, `?limit=`, `?offset=`) |
| GET | `/admin/products/{id}` | admin | Detalha o produto (mesmo inativo) com todas as variantes, inclusive as inativas |
| GET, PUT, DELETE | `/me` | cliente | Consulta, atualiza e remove o próprio cadastro |
| GET | `/cart` | cliente | Consulta o carrinho |
| POST | `/cart/items` | cliente | Adiciona variante ao carrinho |
| PUT, DELETE | `/cart/items/{variant_id}` | cliente | Altera quantidade ou remove item |
| GET | `/orders` | cliente | Lista os pedidos do cliente |
| POST | `/orders` | cliente | Checkout do carrinho |
| GET | `/orders/{id}` | cliente | Detalha um pedido do cliente |
| POST | `/orders/{id}/cancel` | cliente | Cancela um pedido pendente e devolve o estoque |
| POST | `/orders/{id}/pay` | cliente | Pagamento simulado de um pedido pendente |
| GET | `/admin/orders` | admin | Lista os pedidos de todos os clientes (`?status=`, `?limit=`, `?offset=`) |
| GET | `/admin/orders/{id}` | admin | Detalha qualquer pedido |
| PUT | `/admin/orders/{id}/status` | admin | Muda o status do pedido |

## Autenticação

Rotas protegidas esperam o cabeçalho `Authorization: Bearer <session_token>`.

O login é por `username` e senha, sem e-mail. O `username` não diferencia maiúsculas de minúsculas (`Ana` e `ana` são o mesmo usuário) e é guardado em minúsculas.

- A sessão dura 1 hora e o refresh token 15 dias.
- `POST /auth/refresh` gira os tokens: o refresh token só vale uma vez, e a sessão antiga deixa de funcionar.
- Sessões com refresh token vencido são apagadas na partida e depois a cada hora.
- Senhas são guardadas com argon2; tokens, apenas como hash SHA-256.
- O cliente é sempre identificado pelo token, nunca por um id na URL.

## Itens indisponíveis no carrinho

As respostas de carrinho (`GET /cart`, `POST /cart/items`, `PUT` e `DELETE /cart/items/{variant_id}`) sinalizam o que não pode ser comprado agora:

- cada item traz `available` e `unavailable_reason`: `null` quando disponível, `"inactive"` se o produto ou a variante foi desativado, `"insufficient_stock"` se o estoque atual é menor que a quantidade (inclui estoque zero; quantidade igual ao estoque ainda é válida);
- o carrinho traz `can_checkout`: `true` só se há itens e todos estão disponíveis.

O `total` continua somando todos os itens, disponíveis ou não, e nenhum item é removido sozinho. É um retrato do momento: quem decide é o checkout, que segue recusando com `409` se algo mudou entre a leitura do carrinho e a compra.

## Exclusão lógica

`DELETE` em produto ou variante não apaga o registro: marca `active = false`, porque pedidos e carrinhos apontam para eles. Itens inativos somem do catálogo público (`GET /products`, `/products/{id}`, `/products/{id}/variants`) e não entram no carrinho nem no checkout. Para reativar, use o `PUT` com `"active": true`; os ids de quem foi desativado se descobrem em `GET /admin/products?active=false` (produtos) e `GET /admin/products/{id}` (variantes, com o `active` de cada uma). A listagem de admin vem ordenada por nome (e id, em empate) e aceita `limit` (1 a 200, padrão 50) e `offset`; parâmetros inválidos devolvem `422`. A listagem pública não tem ordem definida. O `stock` do `PUT` é um valor absoluto: um checkout simultâneo pode ser sobrescrito.

## Checkout

`POST /orders` roda em uma única transação: revalida que variante e produto estão ativos, baixa o estoque (`UPDATE ... WHERE stock >= quantity`), cria o pedido `pending` com um retrato de nome, SKU e preço de cada item, e esvazia o carrinho. Se qualquer passo falhar, nada é gravado. Compras simultâneas do mesmo item colidem na escrita; a transação perdedora é repetida automaticamente (até 5 tentativas, com pausa aleatória), então quem perde a última unidade recebe `409` e nunca `500`. O cancelamento de pedido usa a mesma repetição.

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

## Moeda

A loja opera em **unidades** (singular: *unidade*), sempre números inteiros e sem subunidade. Os campos de valor são `price`, `unit_price`, `line_total` e `total` (ex.: `"price": 25` = 25 unidades). Toda a API responde em inglês.

## Validação

Os corpos JSON passam pelo extractor `ValidatedJson` (crate `validator`). Corpo malformado ou regra violada devolve `422` com `{"error": "campo: mensagem"}`, sem ecoar o valor enviado (LGPD). Regras: username de 3 a 32 caracteres (letras, números, `.`, `-` e `_`), nomes não vazios (máx. 100), preço e estoque não negativos, quantidade >= 1, data de nascimento fora do futuro, senha de 8 a 128 caracteres.

## Testes

```bash
cargo test
```

Os testes sobem o app completo (rotas, services e schema) com um banco SurrealKV novo num diretório temporário e chamam o `Router` em memória, sem abrir porta. Não tocam em `data/rust-store`. Cobrem autenticação (inclusive refresh simultâneo), catálogo (inclusive o admin de inativos), carrinho (inclusive itens indisponíveis), checkout (rollback e corrida pelo último item), cancelamento, pagamento e rotas de admin de pedidos (inclusive simultâneos) e limpeza de sessões.

## Pagamento

O pagamento é simulado: `POST /orders/{id}/pay` só troca o status de `pending` para `paid`, sem gateway nem dados de cartão. O estoque não muda (já foi baixado no checkout). Só pedido pendente paga; pedido `paid` ou `cancelled` devolve `409`. Não há reembolso, então um pedido pago **não pode ser cancelado**. Pagar e cancelar ao mesmo tempo tem um único vencedor.

## Data do pagamento

Todo pedido traz `paid_at`: o instante em que ele virou `paid`, gravado pelo banco no mesmo comando que troca o status (tanto no `POST /orders/{id}/pay` quanto no `PUT /admin/orders/{id}/status`). É `null` enquanto o pedido não foi pago e em pedido cancelado. Enviar e entregar não alteram a data. Pedidos pagos **antes** do campo existir ficam com `paid_at: null`, porque a data real não foi guardada e não é inventada.

## Pedidos no admin

Ciclo de status: `pending` → `paid` ou `cancelled`; `paid` → `shipped` → `delivered`. `cancelled` e `delivered` são finais. `PUT /admin/orders/{id}/status` recebe `{"status": "paid"}` e devolve `409` para qualquer transição fora do ciclo. Cancelar um pedido pendente devolve o estoque; pedido pago não cancela (sem reembolso). O cliente continua podendo pagar e cancelar o que é seu, mas só enquanto o pedido está `pending`.

`GET /admin/orders` lista do mais recente para o mais antigo, com `limit` (1 a 200, padrão 50) e `offset`. Cada pedido traz só o `customer_id` do dono, sem dados pessoais. Parâmetros inválidos devolvem `422`.

## Limitações conhecidas

Não há gateway de pagamento real nem reembolso.
