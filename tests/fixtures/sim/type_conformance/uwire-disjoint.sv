module tb;
    uwire [1:0] u; assign u[0]=0; assign u[1]=1;
    initial begin #1; $display("%b",u); $finish(0); end
endmodule
