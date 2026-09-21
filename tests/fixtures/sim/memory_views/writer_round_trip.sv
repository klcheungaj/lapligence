// llg-test-fixture: tests/fixtures/sim/memory_views/writer_round_trip.sv
module tb;
    logic [7:0] mem [1:0][2:3];
    initial begin
        mem[0][2] = 8'h11;
        mem[0][3] = 8'h12;
        mem[1][2] = 8'h21;
        mem[1][3] = 8'h22;
        $writememh("round.mem", mem);
        mem[0][2] = 8'hee;
        mem[0][3] = 8'hee;
        mem[1][2] = 8'hee;
        mem[1][3] = 8'hee;
        $readmemh("round.mem", mem);
        $display("w0=%h w1=%h w2=%h w3=%h",
                 mem[0][2], mem[0][3], mem[1][2], mem[1][3]);
    end
endmodule
