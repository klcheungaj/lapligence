// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_runtime_oversize.sv
// IEEE 1800-2009 §11.4.14: a runtime-sized stream larger than its fixed-size
// target is an error rather than a truncation.
module tb;
    logic [7:0] queue_value [$];
    logic [7:0] narrow;
    initial begin
        queue_value = '{8'h44, 8'h55};
        narrow = {>>8{queue_value}};
        $display("unexpected %h", narrow);
    end
endmodule
