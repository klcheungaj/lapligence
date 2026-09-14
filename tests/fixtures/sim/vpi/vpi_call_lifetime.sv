// R22: handles borrowed by each callback expire before the next callback.
module tb;
    logic [7:0] result;
    initial begin
        for (int i = 0; i < 2; i++) begin
            result = $vpi_lifetime(8'h3c);
            $display("result=%0d", result);
        end
        $finish(0);
    end
endmodule
