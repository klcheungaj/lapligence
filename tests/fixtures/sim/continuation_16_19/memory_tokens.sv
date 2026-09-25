// V 17.2.8 / SV 21.4 and integer based-digit padding; shared 2001 grammar.
module memory_token_check #(parameter W=7) (output reg done);
    reg signed [W-1:0] hex_memory[0:7];
    reg [W-1:0] binary_memory[0:7];
    integer bit_index;
    task fail;
        input [255:0] reason;
        begin $display("FAIL token W=%0d %0s", W, reason); $finish(0); end
    endtask
    initial begin
        done = 0;
        $readmemh("tokens.hex", hex_memory);
        $readmemb("tokens.bin", binary_memory);
        for (bit_index=0; bit_index<W; bit_index=bit_index+1) begin
            if (hex_memory[0][bit_index] !== 1'bx || binary_memory[0][bit_index] !== 1'bx)
                fail("leading X fill");
            if (hex_memory[1][bit_index] !== 1'bz || binary_memory[1][bit_index] !== 1'bz)
                fail("leading Z fill");
            if (hex_memory[2][bit_index] !== (bit_index<4 ? 1'bx : 1'b0) ||
                binary_memory[2][bit_index] !== (bit_index<1 ? 1'bx : 1'b0)) fail("0X zero fill");
            if (hex_memory[3][bit_index] !== (bit_index<4 ? (bit_index<3 ? 1'b1 : 1'b0) : 1'bx) ||
                binary_memory[3][bit_index] !== (bit_index<1 ? 1'b1 : 1'bx)) fail("mixed X fill");
            if (hex_memory[4][bit_index] !== (bit_index<4 ? 1'bz : 1'b0) ||
                binary_memory[4][bit_index] !== (bit_index<1 ? 1'bz : 1'b0)) fail("0Z zero fill");
            if (hex_memory[5][bit_index] !== (bit_index<4 ? 1'b0 : 1'bz) ||
                binary_memory[5][bit_index] !== (bit_index<1 ? 1'b0 : 1'bz)) fail("mixed Z fill");
            if (hex_memory[6][bit_index] !== (bit_index<4 ? 1'b1 : 1'b0) ||
                binary_memory[6][bit_index] !== (bit_index<1 ? 1'b1 : 1'b0)) fail("known digit zero fill");
            if (hex_memory[7][bit_index] !== (bit_index<9 ? 1'b1 : 1'b0) ||
                binary_memory[7][bit_index] !== ((bit_index==0 || bit_index==2) ? 1'b1 : 1'b0))
                fail("ordinary packed truncation");
        end
        done = 1;
    end
endmodule
module tb;
    wire [4:0] done;
    memory_token_check #(1) c1(done[0]);
    memory_token_check #(7) c7(done[1]);
    memory_token_check #(8) c8(done[2]);
    memory_token_check #(65) c65(done[3]);
    memory_token_check #(129) c129(done[4]);
    initial begin
        wait (&done);
        $display("MEMORY_TOKENS_PASS");
        $finish(0);
    end
endmodule
