# The mapping rule

Written by tools/api-crate-generator from src/mapping.rs; do not edit.

| Row | C form -> Rust form |
|---|---|
| Scalar | integer or float by value -> the same Rust primitive |
| Bool | BOOL -> bool (any nonzero is true) |
| Enumeration | C enum or flags -> a newtype over its integer, its values as constants |
| Handle | handle typedef (HWND, HMODULE, HANDLE, ...) -> a newtype over usize |
| Guid | GUID, REFGUID, REFIID -> Guid, &Guid |
| StructureByValue | struct by value -> the struct |
| StructureIn | const struct pointer (_In_, _In_opt_) -> &T, Option<&T> |
| StructureInOut | struct pointer _Inout_ -> &mut T |
| ValueOut | _Out_ pointer to a value -> the value, returned |
| ObjectIn | interface pointer in -> &the object kind's associated type (Option for _opt_) |
| BaseObjectIn | pointer to an interface several kinds derive from -> <Interface>Ref enum of those kinds |
| UnknownIn | IUnknown pointer in -> UnknownRef enum: the API's kinds, or Unknown |
| ObjectOut | _COM_Outptr_ -> the owned object kind, returned; an _Out_ of an object not made -> Option |
| BaseObjectOut | out pointer to a base interface -> <Interface>Object enum of the kinds, returned |
| InterfaceById | REFIID riid + void** ppv -> &Guid in, the API's Object enum returned |
| ArrayIn | _In_reads_(n) array + its count -> &[T], the count dropped |
| ArrayFixedIn | array of a fixed count -> &[T; N] |
| ArrayOut | _Out_writes_(n) array, n given in -> n kept, Vec<T> returned |
| ArrayOutCounted | _Out_writes_ array whose count is an _Inout_ pointer -> Vec<T> returned, count dropped |
| BytesIn | const void* + byte count -> &[u8], copied once by the serving glue |
| BytesOut | void* _Out_writes_bytes_(n) -> &impl VolatileBytes, n dropped (its len) |
| BytesOutCounted | void* out + _Inout_ size -> Option<&impl VolatileBytes>, the size returned |
| StringIn | PCSTR in -> &CStr |
| WideStringIn | PCWSTR in -> &[u16] (no terminator) |
| CountDropped | a count another parameter's annotation names -> dropped (the slice's len) |
| StatusResult | HRESULT return -> Result<outs, HResult> |
| FixedArrayField | fixed array field -> [T; N] |
| BytesField | void pointer field -> Vec<u8>, length from an exception line |
| ArrayField | pointer field with a count field -> Vec<T> |
| StringField | PCSTR field -> CString |
| ObjectField | interface pointer field -> Option<owned object kind>, the struct generic over the API |
| UnionField | C union -> a struct of every member, each read from the same bytes |
| LargeInteger | LARGE_INTEGER -> i64 |
| Unmapped | no row: written as &impl VolatileBytes (caller bytes) or Vec<u8>, and listed |
