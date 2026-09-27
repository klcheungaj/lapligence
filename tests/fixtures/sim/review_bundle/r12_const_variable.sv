// SV 6.20.6: const variables initialize once and cannot be assigned afterward.
module tb;
    const int module_value = 7;

    function automatic int local_read();
        const int local_value = 3;
        local_read = module_value + local_value;
    endfunction

    initial begin
`ifdef WRITE_CONST
        module_value = 8;
`else
        $display("const=%0d,%0d", module_value, local_read());
        $finish(0);
`endif
    end
endmodule
