module indexed_leaf(input logic [3:0] tag);
    initial #1 $display("%m=%0d", tag);
endmodule
module tb;
    indexed_leaf offset_array[5:4](8'ha3);
    indexed_leaf negative_array[-2:1](16'h1234);
    indexed_leaf ascending_array[0:3](16'h5678);
    indexed_leaf descending_array[3:0](16'h9abc);
    indexed_leaf matrix[2:1][0:1](16'h1357);
    initial begin
        #2 $finish(0);
    end
endmodule
