`timescale 100s/1fs
module tb;
    initial begin
        #(64'hffff_ffff_ffff_ffff) $finish(0);
    end
endmodule
