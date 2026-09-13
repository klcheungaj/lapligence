// llg-test-fixture: tests/fixtures/sim/process_semantics/function_side_effect.sv
// IEEE 1800-2009 §§9.2.2.2 and 13.4: a legal called-function side effect is
// retained in the always_comb write set without causing a false rejection.
module tb;
    logic a;
    logic helper;
    logic y;

    function automatic logic side_effect(input logic value);
        helper = value;
        side_effect = value;
    endfunction

    always_comb y = side_effect(a);

    initial begin
        a = 1'b0;
        #1 $display("t=%0t helper=%b y=%b", $time, helper, y);
        a = 1'b1;
        #1 $display("t=%0t helper=%b y=%b", $time, helper, y);
        $finish;
    end
endmodule
