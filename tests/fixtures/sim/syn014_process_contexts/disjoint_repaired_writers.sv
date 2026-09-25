// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/disjoint_repaired_writers.sv
// IEEE 1800-2009 §§9.2.2.2, 9.2.2.4, 10.9.1: distinct array elements and
// structure members are separate longest static prefixes, so positional,
// element and member writers in different processes are legal.
module tb;
    typedef struct { logic [7:0] data; bit flag; } record_t;
    record_t value;
    logic [7:0] row [2:0];
    logic [7:0] source [1:0];
    logic [7:0] data_in, flag_in, first;
    logic clk;
    always_comb '{row[2], first} = source;
    always_comb row[0] = data_in;
    always_comb value.data = data_in;
    always_ff @(posedge clk) value.flag <= flag_in[0];
    initial begin
        clk = 0;
        data_in = 8'h11;
        flag_in = 8'h01;
        source = '{8'haa, 8'hbb};
        #1 $display("row2=%h first=%h row0=%h data=%h flag=%b",
                    row[2], first, row[0], value.data, value.flag);
        clk = 1;
        data_in = 8'h22;
        #1 $display("row2=%h first=%h row0=%h data=%h flag=%b",
                    row[2], first, row[0], value.data, value.flag);
        $finish(0);
    end
endmodule
