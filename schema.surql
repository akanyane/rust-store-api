DEFINE TABLE IF NOT EXISTS product SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name        ON product TYPE string;
DEFINE FIELD IF NOT EXISTS description ON product TYPE string;
DEFINE FIELD IF NOT EXISTS active      ON product TYPE bool DEFAULT true;

DEFINE TABLE IF NOT EXISTS variant SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS product     ON variant TYPE record<product>;
DEFINE FIELD IF NOT EXISTS name        ON variant TYPE string;
DEFINE FIELD IF NOT EXISTS sku         ON variant TYPE string;
DEFINE FIELD IF NOT EXISTS price ON variant TYPE int ASSERT $value >= 0;
DEFINE FIELD IF NOT EXISTS stock       ON variant TYPE int ASSERT $value >= 0;
DEFINE FIELD IF NOT EXISTS active      ON variant TYPE bool DEFAULT true;
DEFINE INDEX IF NOT EXISTS variant_sku_unique ON variant FIELDS sku UNIQUE;

-- Identidade e login. O customer reaproveita a MESMA chave do user (user:abc <-> customer:abc).
DEFINE TABLE IF NOT EXISTS user SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS username      ON user TYPE string;
DEFINE FIELD IF NOT EXISTS password_hash ON user TYPE string;
DEFINE FIELD IF NOT EXISTS created_at    ON user TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS role          ON user TYPE string DEFAULT 'customer' ASSERT $value IN ['customer', 'admin'];
DEFINE INDEX IF NOT EXISTS user_username_unique ON user FIELDS username UNIQUE;
-- O DEFAULT não preenche linhas antigas; sem isto, quem já existia falharia na leitura.
UPDATE user SET role = 'customer' WHERE role = NONE;

-- Guarda só o hash (SHA-256) dos tokens; o token em si só existe na resposta do sign-in.
DEFINE TABLE IF NOT EXISTS session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS user               ON session TYPE record<user>;
DEFINE FIELD IF NOT EXISTS token_hash         ON session TYPE string;
DEFINE FIELD IF NOT EXISTS refresh_hash       ON session TYPE string;
DEFINE FIELD IF NOT EXISTS expires_at         ON session TYPE datetime;
DEFINE FIELD IF NOT EXISTS refresh_expires_at ON session TYPE datetime;
DEFINE INDEX IF NOT EXISTS session_token_unique   ON session FIELDS token_hash UNIQUE;
DEFINE INDEX IF NOT EXISTS session_refresh_unique ON session FIELDS refresh_hash UNIQUE;
DEFINE INDEX IF NOT EXISTS session_user ON session FIELDS user;
DEFINE INDEX IF NOT EXISTS session_refresh_expires ON session FIELDS refresh_expires_at;

-- Perfil do cliente. O e-mail agora vive só no user.
DEFINE TABLE IF NOT EXISTS customer SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS first_name ON customer TYPE string;
DEFINE FIELD IF NOT EXISTS last_name  ON customer TYPE string;
DEFINE FIELD IF NOT EXISTS birthday   ON customer TYPE datetime;
REMOVE INDEX IF EXISTS customer_email_unique ON customer;
REMOVE FIELD IF EXISTS email ON customer;

DEFINE TABLE IF NOT EXISTS cart SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS customer ON cart TYPE record<customer>;
DEFINE INDEX IF NOT EXISTS cart_customer_unique ON cart FIELDS customer UNIQUE;

DEFINE TABLE IF NOT EXISTS cart_item SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS cart     ON cart_item TYPE record<cart>;
DEFINE FIELD IF NOT EXISTS variant  ON cart_item TYPE record<variant>;
DEFINE FIELD IF NOT EXISTS quantity ON cart_item TYPE int ASSERT $value >= 1;
DEFINE INDEX IF NOT EXISTS cart_item_cart_variant_unique ON cart_item FIELDS cart, variant UNIQUE;

-- Pedido: guarda o total e, em order_item, um retrato do que foi vendido
-- (nome, SKU e preço da hora), porque a variante pode mudar depois.
DEFINE TABLE IF NOT EXISTS order SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS customer    ON order TYPE record<customer>;
-- OVERWRITE: a lista de status cresceu, e IF NOT EXISTS não atualizaria bancos já criados.
DEFINE FIELD OVERWRITE status ON order TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'paid', 'cancelled', 'shipped', 'delivered'];
DEFINE FIELD IF NOT EXISTS total ON order TYPE int ASSERT $value >= 0;
DEFINE FIELD IF NOT EXISTS created_at  ON order TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS order_customer ON order FIELDS customer;

DEFINE TABLE IF NOT EXISTS order_item SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS order            ON order_item TYPE record<order>;
DEFINE FIELD IF NOT EXISTS variant          ON order_item TYPE record<variant>;
DEFINE FIELD IF NOT EXISTS name             ON order_item TYPE string;
DEFINE FIELD IF NOT EXISTS sku              ON order_item TYPE string;
DEFINE FIELD IF NOT EXISTS unit_price ON order_item TYPE int ASSERT $value >= 0;
DEFINE FIELD IF NOT EXISTS quantity         ON order_item TYPE int ASSERT $value >= 1;
DEFINE INDEX IF NOT EXISTS order_item_order ON order_item FIELDS order;
