// Decision S40-D9: a dynamic array is the actual of an input or inout
// open-array formal. The formal is a copy with the size the array has at the
// call: its range is [0:size-1], its increment -1, and an empty array gives
// [0:-1] with size 0 and no element. An exported function that resizes or
// writes the actual during the call changes neither the handle nor the
// formal's elements, and it sees the actual as it was at the call; when the
// import returns, an inout formal is assigned to the actual, which then has
// the size it had at the call again.
//
// IEEE 1800-2009 7.7 (SystemVerilog-1800-2009.txt L8577-8578):
//   "The rules that govern dynamic array and queue formal arguments also
//   govern the behavior of unpacked dimensions of DPI open array formal
//   arguments (see 7.6)."
// IEEE 1800-2009 35.6.1.1 (L56172-56174):
//   "It is sometimes permissible to pass a dynamic array as an actual
//   argument to an imported DPI subroutine. The rules for passing dynamic
//   array actual arguments to imported DPI tasks and functions are identical
//   to the rules for native SystemVerilog tasks and functions."
// IEEE 1800-2009 35.6.1 (L56128-56129):
//   "For the SystemVerilog side of the interface, the semantics of arguments
//   passing is as if input arguments are passed by copy-in, output arguments
//   are passed by copy-out, and inout arguments were passed by copy-in,
//   copy-out."
// IEEE 1800-2009 20.7 (L34699, L34701-34702, L34704-34705):
//   "For a queue or dynamic array dimension, $left shall return 0." ... "For
//   a queue or dynamic array dimension whose current size is zero, $right
//   shall return -1." ... "For a queue or dynamic array dimension, $increment
//   shall return -1."
// The last line asks about an empty array, which some simulators refuse.
//
// Build S40-D9_dynamic_array_actual.c into a shared library and load it with
// the simulator's DPI library option.
module tb;
    import "DPI-C" function int d9_sum(input int a []);
    import "DPI-C" context function void d9_resize(inout int a []);
    import "DPI-C" function void d9_scale(inout real a []);
    export "DPI-C" function d9_grow;

    int d [];
    int empty [];
    real r [];

    function void d9_grow(input int size);
        $display("export sees %0d %0d %0d size=%0d", d[0], d[1], d[2], d.size());
        d = new[size](d);
        d[size - 1] = 77;
    endfunction

    initial begin
        d = new[3];
        d[0] = 1; d[1] = 2; d[2] = 3;
        $display("sum=%0d", d9_sum(d));
        d9_resize(d);
        $display("d=%0d %0d %0d size=%0d", d[0], d[1], d[2], d.size());
        r = new[2];
        r[0] = 1.5; r[1] = 2.5;
        d9_scale(r);
        $display("r=%g %g", r[0], r[1]);
        $display("sum=%0d", d9_sum(empty));
        $finish;
    end
endmodule
