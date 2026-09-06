# Party action menu overflow

The before frames use master at `a952ebfb64767763cc0e117aeea44a356a06b30d`;
this fix branch started at the same commit. Both captures use the same party,
DVs, cursor, and icon animation frame (10).

Reproduce with:

```sh
cargo run -p pokered-app --example party_menu_capture -- /tmp/party-menu-capture
```

| Language | Before | After |
| --- | --- | --- |
| Chinese | ![前](party-menu-zh-before.png) | ![后](party-menu-zh-after.png) |
| English | ![前](party-menu-en-before.png) | ![后](party-menu-en-after.png) |

The frame now includes all three rows and the full English labels. Menus with
field moves and move-forgetting options also reserve space for the longest label,
the cursor, and both borders. Chinese SWITCH is translated as 交换.
