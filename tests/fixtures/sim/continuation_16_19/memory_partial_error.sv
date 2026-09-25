module tb;
    reg [7:0] memory[0:3];
    integer i;
    initial begin
        for (i=0; i<4; i=i+1) memory[i]=10+i;
        $readmemh("bad_address.hex", memory, 0, 3);
        $display("retained=%02h %02h %02h %02h", memory[0], memory[1], memory[2], memory[3]);
        $finish(0);
    end
endmodule
