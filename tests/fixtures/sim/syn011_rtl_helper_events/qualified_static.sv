// llg-test-fixture: tests/fixtures/sim/syn011_rtl_helper_events/qualified_static.sv
// IEEE 1800-2009 §§9.4.2, 12.4.2, 12.5.3 and 13.4.2: a stateless static
// numeric function is legal in an evaluated event expression. Its qualified
// branches retain their runtime checks while their values stay private.
module tb;
    logic [1:0] trigger;
    int changes;

    function int classify(input logic [1:0] value);
        unique case (value)
            2'b00: classify = 1;
            2'b01: classify = 2;
            2'b10: classify = 3;
            default: classify = 4;
        endcase
    endfunction

    function int priority_class(input logic [1:0] value);
        priority if (value[1])
            priority_class = 4;
        else if (value[0])
            priority_class = 5;
        else
            priority_class = 6;
    endfunction

    always @(classify(trigger) or priority_class(trigger))
        changes = changes + 1;

    initial begin
        trigger = 2'b00;
        #1 trigger = 2'b11;
        #1 $display("qualified_static changes=%0d classify=%0d priority=%0d",
                    changes, classify(trigger), priority_class(trigger));
        $finish(0);
    end
endmodule
