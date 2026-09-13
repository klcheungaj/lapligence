module tb;
    logic [7:0] observed;

    task automatic delayed(output logic [7:0] value);
        value = 8'd1;
        #5 value = 8'd2;
    endtask

    task automatic immediate_disable(output logic [7:0] value);
        value = 8'd1;
        disable immediate_disable;
        value = 8'd2;
    endtask

    initial begin
        observed = 8'd9;
        immediate_disable(observed);
        if (observed !== 8'd9) begin
            $display("FAIL immediate_disable_copyout got=%0d", observed);
            $finish;
        end
        fork
            delayed(observed);
        join_none
        #1 disable delayed;
        #1;
        if (observed !== 8'd9) begin
            $display("FAIL disabled_delayed_output_copyout got=%0d", observed);
            $finish;
        end
        $display("PASS disabled_delayed_output_copyout");
        $finish;
    end
endmodule
