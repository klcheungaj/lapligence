// llg-test-fixture: tests/fixtures/sim/memory_views/selected_slice.sv
module tb;
    logic [7:0] mem [0:1][0:2][4:5];
    initial begin
        mem[0][0][4] = 8'hee;
        mem[0][0][5] = 8'hee;
        mem[1][2][4] = 8'hee;
        mem[1][2][5] = 8'hee;
        $readmemh("slice.mem", mem[1][2]);
        $display("s0=%h s1=%h other=%h",
                 mem[1][2][4], mem[1][2][5], mem[0][0][4]);
    end
endmodule
