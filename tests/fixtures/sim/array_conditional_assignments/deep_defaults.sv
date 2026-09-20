// Every repeated operand survives through multiple synthesized default rows.
module tb;
    logic [64:0] cube [1:0][-1:1][3:0];
    logic fill_bit;
    initial begin
        fill_bit = 1'b1;
        cube = '{default: '{default: fill_bit}};
        fill_bit = 1'b0;
        foreach (cube[i,j,k]) begin
            if (cube[i][j][k] !== {65{1'b1}})
                $fatal(1, "shared one default lost an array or packed position");
        end
        fill_bit = 1'bx;
        cube = '{default: '{default: fill_bit}};
        fill_bit = 1'b0;
        foreach (cube[i,j,k]) begin
            if (cube[i][j][k] !== {65{1'bx}})
                $fatal(1, "shared X default lost a position");
        end
        fill_bit = 1'bz;
        cube = '{default: '{default: fill_bit}};
        fill_bit = 1'b0;
        foreach (cube[i,j,k]) begin
            if (cube[i][j][k] !== {65{1'bz}})
                $fatal(1, "shared Z default lost a position");
        end
        $display("deep_defaults passed");
        $finish(0);
    end
endmodule
