// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/pruning.sv
// SIM-047 step 6: storage that no process reads or writes is still dumped
// with its value; the dump is identical with and without the unused-storage
// pass.
`timescale 1ns/1ns
module sink (input logic [3:0] unused_in, output logic [3:0] constant_out);
    assign constant_out = 4'ha;
endmodule

module tb;
    logic [3:0] never_used;
    logic [3:0] written_only;
    logic [3:0] read_only = 4'h5;
    wire [3:0] undriven;
    wire [3:0] from_sink;
    logic [3:0] counter;
    logic [7:0] table_mem [0:1];
    real unused_real;
    sink u(.unused_in(counter), .constant_out(from_sink));

    initial begin
        counter = 4'h0;
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        repeat (3) begin
            #1 counter = counter + 4'h1;
            written_only = counter;
        end
        $strobe("%0t counter=%h read_only=%h from_sink=%h", $time, counter, read_only,
                from_sink);
        #1 $finish;
    end
endmodule
