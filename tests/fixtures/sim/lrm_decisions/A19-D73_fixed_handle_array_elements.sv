// Decision A19-D73: the elements of a one-dimensional fixed-size array of
// strings, chandles or class handles are variables (vpiStringVar,
// vpiChandleVar, vpiClassVar) with vpiArrayMember TRUE. They are reached by
// vpiVarSelect of the array (in declaration order), by vpi_handle_by_index
// with the declared index (an index outside the range gives NULL and an
// error) and by name. An element is named after the array and its declared
// index, its vpiParent is the array, its vpiModule and vpiScope are those of
// the array, and vpiIndex gives the declared index both as a handle and as a
// one-item iteration. A string element has a string value that
// vpi_put_value sets; a class handle element reaches its object through
// vpiClassObj (NULL while it holds none).
//
// IEEE 1800-2009 37.17 detail 18 (SystemVerilog-1800-2009.txt L58780):
// "If a variable is an element of an array (the vpiArrayMember property is
// TRUE), the vpiIndex iterator shall return the indexing expressions that
// select that specific variable out of the array." Industry-standard
// behaviour, except that some simulators give an empty vpiIndex iteration
// for a class handle element that holds no object although
// vpi_handle(vpiIndex) answers it; llg follows detail 18.
//
// Build A19-D73_fixed_handle_array_elements.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d73()` declared as a system
// task).
class C;
    int x;
endclass

module tb;
    string sa [0:1];
    chandle ca [2:3];
    C cla [1:0];
    initial begin
        sa[0] = "a";
        sa[1] = "bc";
        cla[1] = new;
        cla[1].x = 7;
        $d73;
        $display("sa[0]=%s", sa[0]);
        $finish;
    end
endmodule
