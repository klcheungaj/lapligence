// llg-test-fixture: tests/fixtures/sim/partial_features/stop_resume.sv
// IEEE 1364-2001 §17.4.2 / IEEE 1800-2009 §20.2: `$stop` suspends the
// issuing process without discarding same-time or future scheduler state.
`timescale 1ns/1ps
module tb;
    reg [7:0] value;

    task automatic nested_stop;
        begin
            $display("CHECK: nested before");
            $stop(0);
            $display("CHECK: nested after t=%0t", $time);
        end
    endtask

    initial begin
        value = 0;
        $display("CHECK: before");
        nested_stop();
        $display("CHECK: resumed t=%0t", $time);
        #2 $display("CHECK: pending value=%0d t=%0t", value, $time);
        #1 $finish(0);
    end

    initial begin
        #1 value <= 8'h5a;
    end

    final begin
        $display("CHECK: final value=%h t=%0t", value, $time);
    end
endmodule
