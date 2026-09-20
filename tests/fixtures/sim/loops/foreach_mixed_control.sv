// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_control.sv
// IEEE 1800-2009 12.7.3, 12.8: one source loop, signed bounds, lexical iterators.
module tb;
    logic [2:-1] rows [1:0];
    logic [2147483646:2147483647] high [0:1];
    logic [-2147483647:-2147483648] low [0:0];
    logic [-2147483648:-2147483648] singleton [0:0];
    integer i;
    integer visits;
    integer nested;
    integer endpoints;

    initial begin
        i = 55;
        visits = 0;
        foreach (rows[i,j]) begin
            if (j == 1) continue;
            if (i == 0 && j == 0) break;
            $write("%0d:%0d ", i, j);
            visits++;
        end
        nested = 0;
        foreach (rows[i,j]) begin
            for (int k = 0; k < 3; k++) begin
                if (k == 1) break;
                nested++;
            end
            if (j == 0) continue;
            nested += 10;
        end
        endpoints = 0;
        foreach (high[a,b]) begin
            endpoints++;
            if (endpoints > 4) $fatal(1, "high bound wrapped");
        end
        foreach (low[a,b]) begin
            endpoints++;
            if (endpoints > 6) $fatal(1, "low bound wrapped");
        end
        foreach (singleton[a,b]) endpoints++;
        $display("visits=%0d outer=%0d nested=%0d endpoints=%0d", visits, i, nested, endpoints);
    end
endmodule
