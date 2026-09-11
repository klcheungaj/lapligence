module tb;
    logic [15:8] down[0:1];
    logic [-4:3] up[1:0][-1:0];
    bit [7:0] two[0:0];
    int base_index;
    initial begin
        down[0] = 8'ha5;
        up[1][-1] = 8'h96;
        base_index = 10;
        $display("read %h %b", down[0][base_index +: 4], up[1][-1][-3 +: 3]);
        down[0][base_index +: 4] = 4'h3;
        up[1][-1][-3 +: 3] = 3'b101;
        up[0][0] = '0;
        up[0][0][2 -: 4] = 4'ha;
        down[1] = '0;
        down[1][8 +: 4] = 2.5;
        two[0] = '0;
        two[0][4 +: 4] = 4'bxz10;
        $display("write %h %h %h %h %h", down[0], up[1][-1], up[0][0], down[1], two[0]);
        $finish;
    end
endmodule
