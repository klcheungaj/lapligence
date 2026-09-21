// llg-test-fixture: tests/fixtures/sim/memory_views/selected_row.sv
module tb;
    logic [7:0] mem [0:1][0:2];
    initial begin
        mem[0][0] = 8'hee;
        mem[0][1] = 8'hee;
        mem[0][2] = 8'hee;
        mem[1][0] = 8'hee;
        mem[1][1] = 8'hee;
        mem[1][2] = 8'hee;
        $readmemh("row.mem", mem[1]);
        $display("r0=%h r1=%h r2=%h untouched=%h",
                 mem[1][0], mem[1][1], mem[1][2], mem[0][0]);
    end
endmodule
