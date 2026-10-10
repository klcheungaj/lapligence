// Decision S40-D1: an imported subroutine sees each output formal at its
// type's default value, whatever the actual held before the call: every bit
// of a 4-state output is X, a 2-state output is 0, and unpacked or
// structured outputs hold those values per element. The lines the C side
// prints (labelled `llg`) are llg's choice; the SystemVerilog lines are what
// every conforming simulator prints.
//
// IEEE 1800-2009 35.5.1.2 (SystemVerilog-1800-2009.txt L55643-55644):
//   "The imported function shall not assume anything about the initial values
//   of formal output arguments. The initial values of output arguments are
//   undetermined and implementation dependent."
// IEEE 1800-2009 H.6.3 (L72017-72018):
//   "The initial values of formal arguments specified in SystemVerilog as
//   output are undetermined and implementation dependent."
//
// Build S40-D1_output_initial_values.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    typedef struct { int i; logic [3:0] l; } rec_t;
    import "DPI-C" function void d1_outputs(output logic [40:0] lv, output bit [40:0] bv,
                                            output int i, output logic s,
                                            output rec_t r, output bit [3:0] ba [2]);
    logic [40:0] lv;
    bit [40:0] bv;
    int i;
    logic s;
    rec_t r;
    bit [3:0] ba [2];

    initial begin
        lv = '0;
        bv = '1;
        i = 5;
        s = 1'b1;
        r.i = 9;
        r.l = 4'b0101;
        ba[0] = 4'hf;
        ba[1] = 4'h3;
        d1_outputs(lv, bv, i, s, r, ba);
        $display("lv=%h bv=%h i=%0d s=%b r=%0d,%b ba=%h,%h", lv, bv, i, s, r.i, r.l, ba[0],
                 ba[1]);
        $finish;
    end
endmodule
