// llg-test-fixture: G1-34 rtl_composition_gate.
// Declaration-initialized memory, whole-array copy, memory slice assignment,
// runtime index loops and a function that reads memory by index.
module tb;
    logic [7:0] rom [0:7] = '{8'h10, 8'h20, 8'h30, 8'h40,
                               8'h50, 8'h60, 8'h70, 8'h80};
    logic [7:0] ram [0:3] = '{default: 8'h00};
    logic [7:0] copy [0:7];
    logic [7:0] paired [0:3];

    function automatic logic [7:0] lookup(input logic [2:0] idx);
        lookup = rom[idx];
    endfunction

    initial begin
        copy = rom;
        ram[1:2] = rom[2:3];
        for (int k = 0; k < 4; k = k + 1)
            paired[k] = rom[2*k] + rom[2*k + 1];
        $display("rom0=%h rom7=%h copy3=%h", rom[0], rom[7], copy[3]);
        $display("ram=%h %h %h %h", ram[0], ram[1], ram[2], ram[3]);
        $display("lookup=%h", lookup(3'd5));
        $display("paired=%h %h %h %h", paired[0], paired[1], paired[2], paired[3]);
        $finish(0);
    end
endmodule
