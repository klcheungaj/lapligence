// Decision A19-D3: an element of an unpacked array of unpacked structures
// is a vpiStructVar like a variable of that structure type: vpiSize is the
// number of fields, vpi_get_value and vpi_put_value on the whole element
// fail with vpiError and change nothing, and its members (vpiMember, or a
// name such as `tb.arr[1].b`) read and write their fields. llg formerly
// read and wrote such an element as one packed vector of all its fields.
//
// IEEE 1800-2009 37.17 detail 9 (SystemVerilog-1800-2009.txt
// L58763): "For unpacked structures and unions, the size returned indicates
//   the number of fields in the structure or union."
// 37.24 detail 1 (L59271-59272): "vpi_get_value()/vpi_put_value() cannot be
//   used to access values of entire unpacked structures and unpacked
//   unions."
// The text does not say whether an array element is covered; llg applies
// both rules to it.
//
// Build A19-D3_unpacked_struct_elements.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d3()` declared as a system task).
module tb;
    typedef struct { int a; logic [7:0] b; } pair_t;
    pair_t arr [1:0];
    initial begin
        arr[0] = '{a: 1, b: 8'h02};
        arr[1] = '{a: 3, b: 8'h04};
        $d3;
        #1;
        $display("arr[1].a=%0d arr[1].b=%h arr[0].b=%h", arr[1].a, arr[1].b, arr[0].b);
        $finish;
    end
endmodule
