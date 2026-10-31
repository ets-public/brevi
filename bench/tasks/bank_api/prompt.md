Stdin holds HTTP-style requests for a small bank API, one per line: `METHOD PATH` optionally followed by a space and a JSON object body. For each request print one line: the status code, a space, and a JSON response body written compactly (no spaces, keys in the order shown below).

Accounts have an id (integers from 1, in creation order), an owner and a balance in cents (an integer).

- `POST /accounts {"owner": NAME}`: create an account with balance 0. Respond `201 {"id":ID,"owner":NAME,"balance":0}`. If owner is missing, not a string, or empty, respond `400 {"error":"owner required"}`.
- `GET /accounts/ID`: respond `200` with the account object as above, or `404 {"error":"no such account"}`.
- `POST /accounts/ID/deposit {"amount": N}`: N must be a positive integer, else `400 {"error":"bad amount"}`. Respond `200` with the updated account.
- `POST /accounts/ID/withdraw {"amount": N}`: as deposit, but if N is more than the balance respond `409 {"error":"insufficient funds"}`.
- `POST /transfer {"from": ID, "to": ID, "amount": N}`: both accounts must exist (else 404 as above), they must differ (else `400 {"error":"same account"}`), the amount is checked as above, then funds as above. Respond `200 {"from":BALANCE,"to":BALANCE}` with both new balances.
- `GET /accounts`: respond `200` with a JSON array of all accounts sorted by balance, highest first, then by id.
- Any other method and path: `404 {"error":"not found"}`. A body that is not valid JSON: `400 {"error":"bad json"}`.

Check in this order: route, then JSON body, then account existence, then the other rules. A route with a non-numeric ID is "not found". Names contain only letters and spaces.
