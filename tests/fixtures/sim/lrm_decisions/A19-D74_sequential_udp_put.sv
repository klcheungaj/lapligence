// Decision A19-D74: vpi_put_value to a sequential UDP instance sets its
// state with no delay whatever the delay of the instance: the output takes
// the value in the same time step, and a pending delayed transition of the
// output is dropped. The state persists: later input changes continue from
// it. Only vpiNoDelay is accepted; other flags and a put to a
// combinational UDP are errors. Z sets X, as a table output is 0, 1 or X.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64235-64237):
// "Sequential UDPs shall be set to the indicated value with no delay
// regardless of any delay on the primitive instance. Putting values to UDP
// instances shall be done using the vpiNoDelay flag. Attempting to use the
// other delay modes shall result in an error." Some simulators publish the
// value through the instance delay; llg follows the clause text.
//
// Build A19-D74_sequential_udp_put.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d74(int)` declared as a system task).
`timescale 1ns/1ns
primitive dff (q, d, clk);
    output q;
    reg q;
    input d, clk;
    initial q = 0;
    table
        0 r : ? : 0;
        1 r : ? : 1;
        ? f : ? : -;
        * ? : ? : -;
    endtable
endprimitive

primitive latch (q, d, en);
    output q;
    reg q;
    input d, en;
    table
        0 1 : ? : 0;
        1 1 : ? : 1;
        ? 0 : ? : -;
    endtable
endprimitive

primitive inv (q, a);
    output q;
    input a;
    table
        0 : 1;
        1 : 0;
    endtable
endprimitive

module tb;
    reg d = 0, clk = 0, en = 0;
    wire q1, q2, q3, q4;
    dff #5 u1 (q1, d, clk);
    dff u2 (q2, d, clk);
    latch u3 (q3, d, en);
    inv u4 (q4, d);
    // The start-up transitions are outside this decision.
    always @(q1, q2, q3)
        if ($time > 6) $display("%0t q1=%b q2=%b q3=%b", $time, q1, q2, q3);
    initial begin
        #10 d = 1;
        clk = 1;
        #2 $d74(1);
        #10 $d74(2);
        #10 clk = 0;
        #10 $d74(3);
        #10 d = 0;
        clk = 1;
        #20 $finish;
    end
endmodule
