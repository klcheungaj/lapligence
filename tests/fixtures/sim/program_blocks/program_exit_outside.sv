// IEEE 1800-2009 24.7: a module-origin call is ignored.
module tb;
    initial begin
        $exit;
        $display("module survived exit");
        $finish(0);
    end
endmodule
