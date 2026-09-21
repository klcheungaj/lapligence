# SYN-032 library and configuration fixtures

These files exercise the bounded public library flow. `root.map` includes
`cells.map`, which assigns two same-named module definitions to distinct
libraries. `top.sv` supplies the ordinary design unit and `config.sv` selects
the library definitions for the two instances while the default liblist picks
the third definition.
