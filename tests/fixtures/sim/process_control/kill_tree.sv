// llg-test-fixture: tests/fixtures/sim/process_control/kill_tree.sv
// IEEE 1800-2009 §§9.6.3 and 9.7: killing a process recursively cleans its
// descendants, while an independent delayed nonblocking assignment survives.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    process owner;
    logic [7:0] killed_value;
    logic [7:0] independent_value;

    initial begin
        killed_value = 8'h00;
        independent_value = 8'h00;
        fork
            begin
                owner = process::self();
                fork
                    begin
                        #2 killed_value <= 8'ha5;
                        #10 killed_value <= 8'h5a;
                    end
                join_none
                #20;
            end
        join_none
        #0;
        independent_value <= #2 8'h3c;
        #1;
        owner.kill();
        #3;
        $display(
            "kill owner=%0d killed=%h independent=%h",
            owner.status(),
            killed_value,
            independent_value
        );
        $finish(0);
    end
endmodule
