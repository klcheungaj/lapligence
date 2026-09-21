// llg-test-fixture: tests/fixtures/sim/memory_views/packed_struct.sv
module tb;
    typedef struct packed {
        logic [3:0] high;
        logic [3:0] low;
    } word_t;
    word_t mem [0:1];
    logic [7:0] s0;
    logic [7:0] s1;
    initial begin
        $readmemh("struct.mem", mem);
        s0 = mem[0];
        s1 = mem[1];
        $display("s0=%h s1=%h", s0, s1);
    end
endmodule
