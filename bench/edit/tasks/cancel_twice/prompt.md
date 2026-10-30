Bug report: cancelling an order twice puts its stock back twice. A second `cancel` of the same order must print `error: order ID already cancelled` (with the order's id) and change nothing.
