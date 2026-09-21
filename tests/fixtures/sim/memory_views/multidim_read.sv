// llg-test-fixture: tests/fixtures/sim/memory_views/multidim_read.sv
module tb;
    logic [7:0] mem [1:0][2:3];
    initial begin
        mem[0][2] = 8'hee;
        mem[0][3] = 8'hee;
        mem[1][2] = 8'hee;
        mem[1][3] = 8'hee;
        $readmemh("multi.mem", mem);
        $display("m0_2=%h m0_3=%h m1_2=%h m1_3=%h",
                 mem[0][2], mem[0][3], mem[1][2], mem[1][3]);
    end
endmodule
