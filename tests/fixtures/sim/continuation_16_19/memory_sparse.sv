module tb;
    reg [7:0] memory[3:0];
    integer i;
    initial begin
        for (i=0; i<4; i=i+1) memory[i]=7;
        $readmemh("sparse.hex", memory);
        $display("sparse=%02h %02h %02h %02h", memory[3], memory[2], memory[1], memory[0]);
        $finish(0);
    end
endmodule
