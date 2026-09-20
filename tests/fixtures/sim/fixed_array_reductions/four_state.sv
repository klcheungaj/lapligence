module tb;
    logic [3:0] data [0:1];
    logic [3:0] singleton [5:5];
    initial begin
        data[0] = 4'b10xz; data[1] = 4'b0110;
        $display("sum=%b product=%b and=%b or=%b xor=%b", data.sum(), data.product(),
                 data.and(), data.or(), data.xor());
        singleton[5] = 4'bzzzz;
        $display("singleton=%b,%b,%b,%b,%b", singleton.sum(), singleton.product(),
                 singleton.and(), singleton.or(), singleton.xor());
        singleton[5] = 4'b10xz;
        $display("single_x=%b clean=%0d", singleton.sum(), singleton.sum() with (int'(item)));
        data[0] = 'x; data[1] = '0;
        $display("zero_and=%b unknown_product=%b", data.and(), data.product());
        data[1] = '1;
        $display("one_or=%b", data.or());
        $finish(0);
    end
endmodule
