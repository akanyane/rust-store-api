# Changelog

## [0.2.0](https://github.com/akanyane/rust-store-api/compare/rust-store-api-v0.1.0...rust-store-api-v0.2.0) (2026-10-12)


### Features

* API de loja com produtos, variantes, carrinho, auth e pedidos ([4226d13](https://github.com/akanyane/rust-store-api/commit/4226d13d2745aed20251835cf545d29c0c879949))
* busca de pedidos por número ou cliente no admin ([6d04312](https://github.com/akanyane/rust-store-api/commit/6d0431290c10827f34aac0b0c38513c11a5a1c9a))
* cancelamento de pedido com devolução de estoque ([ed5e18e](https://github.com/akanyane/rust-store-api/commit/ed5e18ebf43add08f02d0eb461c61be4f9facc14))
* carrinho sinaliza itens indisponíveis e can_checkout ([625d814](https://github.com/akanyane/rust-store-api/commit/625d814fd57588fec141e09220e51357a09ffb89))
* created_at nos produtos e ordenação por mais recentes no admin ([1bceb09](https://github.com/akanyane/rust-store-api/commit/1bceb09b68fbb4d70c744eb5ead2279db97beb3c))
* detalhe de cliente no admin ([985ff2b](https://github.com/akanyane/rust-store-api/commit/985ff2b531a5bf0449b5bad5cb39a045c8e48b74))
* exportação de pedidos em CSV no admin ([5037a37](https://github.com/akanyane/rust-store-api/commit/5037a37f180295e95e00acddbd147d7c33f4fa8b))
* GET /admin/stats com os números do painel ([60533d0](https://github.com/akanyane/rust-store-api/commit/60533d0b21584a66663c18eeb1ba07379c9045a3))
* GET /auth/session com username e papel da sessão ([0918d74](https://github.com/akanyane/rust-store-api/commit/0918d74ab982f30fcc2dbbda01b21bea09df9dea))
* limpeza periódica de sessões expiradas ([743f720](https://github.com/akanyane/rust-store-api/commit/743f720a99f5c6b9b114c754b97f55c9e1d4c900))
* lista de clientes no admin ([c6b5d12](https://github.com/akanyane/rust-store-api/commit/c6b5d12f8027d41b7390a5fe29188402afbc4610))
* listagem de admin para produtos inativos ([0487afc](https://github.com/akanyane/rust-store-api/commit/0487afc6eb7766be8f5d2dca4c62f359cfa42206))
* login por username no lugar de e-mail ([5ab3e13](https://github.com/akanyane/rust-store-api/commit/5ab3e13d893f1b038d18b991d5ae5446e68f720a))
* ordem definida na listagem pública de produtos ([e9f8757](https://github.com/akanyane/rust-store-api/commit/e9f87577a9095b0d3e1e08da5bc7b82b78ce39c8))
* ordem definida nas variantes (nome e SKU) ([442cd95](https://github.com/akanyane/rust-store-api/commit/442cd950dacaf194681984a62cf16c587a142630))
* ordenação de texto sem diferenciar maiúsculas nem acentos ([1664ce4](https://github.com/akanyane/rust-store-api/commit/1664ce48415bb9b8072087229185d18c977eed88))
* pagamento simulado de pedidos ([91b7d1b](https://github.com/akanyane/rust-store-api/commit/91b7d1b81a0937387e97ce3cd96950afce80eb42))
* paginação opcional e X-Total-Count na listagem pública de produtos ([8519c73](https://github.com/akanyane/rust-store-api/commit/8519c73e85a2b1d7f641c2c6660c5fdd81c92894))
* paid_at registra a data do pagamento do pedido ([99bd65c](https://github.com/akanyane/rust-store-api/commit/99bd65cb4fbc7fbc8c8f71c42653a7a4900a0a18))
* price_from e price_to na listagem pública de produtos ([7339461](https://github.com/akanyane/rust-store-api/commit/73394610d8bf3079fd1e51c4107b0a04b4972a2f))
* product_name nos itens do carrinho e dos pedidos ([81fe8df](https://github.com/akanyane/rust-store-api/commit/81fe8df79cec072c785e0a307a210597501a74bd))
* PUT e DELETE (exclusão lógica) de produto e variante ([7ee26dd](https://github.com/akanyane/rust-store-api/commit/7ee26dd9a883e1fc21cdeb89119c81099b971752))
* refresh de token com rotação de sessão ([00cdfb0](https://github.com/akanyane/rust-store-api/commit/00cdfb07145f088b310f3bb5e22941aff3e9b4a1))
* rotas de admin para pedidos com ciclo shipped e delivered ([a1f3e0b](https://github.com/akanyane/rust-store-api/commit/a1f3e0b9bd7554d50a4b8d719ddb67ca295a4ceb))
* validação de entrada, moeda unidades e API em inglês ([b34b827](https://github.com/akanyane/rust-store-api/commit/b34b827a97b0770fead01bb4eb23edbf4a4e1fb4))
* X-Total-Count nas listagens de admin de produtos e pedidos ([b178100](https://github.com/akanyane/rust-store-api/commit/b178100a7f9ede350272d210f60815ffd3445871))


### Bug Fixes

* repetir transação em conflito de escrita no checkout e no cancelamento ([31ff6b6](https://github.com/akanyane/rust-store-api/commit/31ff6b67cd88b4ecbe7cb5df9498f43f6532acb1))
