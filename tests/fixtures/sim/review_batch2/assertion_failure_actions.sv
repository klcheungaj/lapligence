// R26: failure accounting is independent of an explicit or default action.
module tb;
    bit clk = 0;
    a_default: assert property (@(posedge clk) 1'b0);
    a_null: assert property (@(posedge clk) 1'b0) else ;
    a_display: assert property (@(posedge clk) 1'b0) else $display("handled");
    a_error: assert property (@(posedge clk) 1'b0) else $error("EXPLICIT_ERROR");
    initial begin
        #1 clk = 1;
        #1 $finish(2);
    end
endmodule
