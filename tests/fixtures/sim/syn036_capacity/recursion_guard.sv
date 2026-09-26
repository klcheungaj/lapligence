// llg-test-fixture: tests/fixtures/sim/syn036_capacity/recursion_guard.sv
// IEEE 1364-2001 §10.3.1 / IEEE 1800-2009 §13.4.2 allow this recursion;
// exceeding the selected 256-activation budget must diagnose resource use.
module tb;
    integer result;
    function automatic integer descend(input integer remaining);
        if (remaining == 0)
            descend = 0;
        else
            descend = 1 + descend(remaining - 1);
    endfunction

    initial begin
        result = descend(256);
        if (result !== 32'hxxxxxxxx) $display("FAIL recursion guard default");
        $finish(0);
    end
endmodule
