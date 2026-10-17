// Document metadata the author declares with `cnd.document`, and a listing
// title from `cnd.code`. The marker renders nothing and is no node.
#import "vendor/cnd.typ"
#set document(title: "Release notes", author: "typst-cnd")

#cnd.document(status: "Approved", version: "1.2", extra: (client: "ACME"))

= Release notes

A short paragraph before the listing.

#cnd.code(title: "main.py")[```python
print("hello")
```]

A listing without a title:

```python
print("plain")
```
