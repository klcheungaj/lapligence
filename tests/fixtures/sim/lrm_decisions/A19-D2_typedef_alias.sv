// Decision A19-D2: vpiName and vpiTypedefAlias of typespecs.
// - A typespec reached from a declaration made with a typedef has the
//   typedef's name; a built-in type, or an inline type, has a NULL name.
// - A typedef of another typedef (`colors`) has a vpiTypedefAlias handle to
//   that typedef's typespec, whose vpiTypedefAlias is NULL.
// - A typedef of a type that has a keyword name (`time`, `int`) has a
//   vpiTypedefAlias handle to the built-in typespec, with a NULL name.
// - A typedef of a type written with packed dimensions (`logic [3:0]`) has
//   a NULL vpiTypedefAlias: it is neither another typedef nor a predefined
//   type of the language (llg choice; the text is silent).
//
// IEEE 1800-2009 37.23 detail 1 (SystemVerilog-1800-2009.txt
// L59170-59174):
//   "If a typespec denotes a type that has a user-defined typedef, the
//    vpiName property shall return the name of that type; otherwise, except
//    in the case of a class typespec (see 37.28), the vpiName property shall
//    return NULL. Consequently the vpiName property returns NULL for any
//    SystemVerilog built-in type. If the typespec denotes a type with a
//    typedef that creates an alias of another typedef, then the
//    vpiTypedefAlias of the typespec shall return a non-null handle, which
//    represents the handle to the aliased typedef."
// 37.23 detail 6 (L59200-59206): for `typedef time my_time;`
//   "The vpiTypedefAlias applied to handle h1 shall return a typespec handle
//    h2 to the predefined type "time"."
//
// Build A19-D2_typedef_alias.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d2()` declared as a system task).
module tb;
    typedef enum bit [0:2] {red, yellow, blue} primary_colors;
    typedef primary_colors colors;
    typedef time my_time;
    typedef int count_t;
    typedef logic [3:0] nib_t;
    typedef nib_t nib2_t;
    colors c;
    my_time t;
    count_t n;
    nib_t a;
    nib2_t b;
    logic [3:0] raw;
    initial begin
        $d2;
        $finish;
    end
endmodule
