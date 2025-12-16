# Smart Contract Examples

Here are examples of smart contracts you can deploy to your mini-blockchain using the `POST /api/contracts` endpoint.

## 1. Simple Calculator (Addition)
Takes two arguments and returns their sum.

**Source Code:**
```asm
; Load first argument
ARG 0
; Load second argument
ARG 1
; Add them
ADD
; Return result
RETURN
```

**Usage:**
- Deploy via API
- Call with `{"args": [10, 32]}`
- Result: `42`

---

## 2. Persistent Counter
Increments a value stored in the contract's state every time it's called.

**Source Code:**
```asm
; Key for storage (0)
PUSH 0
; Load current value from storage (defaults to 0)
SLOAD

; Add 1 to the value
PUSH 1
ADD

; Duplicate result (one for storage, one for return)
DUP

; Prepare storage key (0)
PUSH 0
; Swap to get order: [key, value]
SWAP

; Store new value
SSTORE

; Return the new value
RETURN
```

**Usage:**
- Deploy via API
- Call 1: Result `1`
- Call 2: Result `2`
- Call 3: Result `3`

---

## 3. High Number Checker
Returns 1 (true) if the argument is greater than 100, otherwise 0 (false).

**Source Code:**
```asm
; Load argument
ARG 0
; Load threshold
PUSH 100

; Check if arg > 100
GT

; Return result (1 or 0)
RETURN
```

**Usage:**
- Call with `[50]` -> Result `0`
- Call with `[150]` -> Result `1`

---

## 4. Max Function (Control Flow)
Returns the larger of two numbers.

**Source Code:**
```asm
ARG 0           ; a
ARG 1           ; b
DUP             ; b
ARG 0           ; a
LT              ; is a < b?
JUMPI return_b  ; if true, jump to return_b

; return_a
ARG 0
RETURN

:return_b
ARG 1
RETURN
```
