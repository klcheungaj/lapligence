// SIM-008: `$value$plusargs` may store into packed elements of queues,
// dynamic arrays and associative arrays (SV 21.6). The element is written
// only when the plusarg matches, so an unmatched associative entry is not
// created. Run with +N=4 +H=1f.
module tb;
    int q[$] = '{0};
    logic [7:0] aa[string];
    int d[];
    int found;
    initial begin
        d = new[1];
        found = $value$plusargs("N=%d", q[0]);
        found += $value$plusargs("H=%h", aa["h"]);
        found += $value$plusargs("M=%d", aa["m"]);
        found += $value$plusargs("N=%d", d[0]);
        $display("%0d %0d %0d %0d %0d", found, q[0], aa["h"], aa.exists("m"), d[0]);
    end
endmodule
