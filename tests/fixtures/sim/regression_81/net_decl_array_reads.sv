module tb;
    logic [7:0] memory [0:1];
    logic index;
    wire [7:0] value = memory[index];
    initial begin
        memory[0] = 11;
        memory[1] = 22;
        index = 0;
        #1;
        $display("value=%0d", value);
        memory[0] = 33;
        #1;
        $display("value=%0d", value);
        index = 1;
        #1;
        $display("value=%0d", value);
        memory[1] = 44;
        #1;
        $display("value=%0d", value);
        $finish(0);
    end
endmodule
