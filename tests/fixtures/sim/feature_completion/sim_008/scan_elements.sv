// SIM-008: `$sscanf` destinations may be packed elements of queues, dynamic
// arrays and associative arrays (SV 21.3.4.3). Each binds the selected element
// when the call starts; destinations the scan does not reach are not written,
// so an associative entry is created only when its item is converted.
module tb;
    int q[$] = '{0, 0};
    int d[];
    int aa[int];
    logic [7:0] sa[string];
    int n;
    initial begin
        d = new[2];
        n = $sscanf("5 6 7 41", "%d %d %d %h", q[1], d[0], aa[3], sa["k"]);
        $display("%0d %0d %0d %0d %0d", n, q[1], d[0], aa[3], sa["k"]);
        n = $sscanf("9", "%d %d", aa[7], aa[8]);
        $display("%0d %0d %0d %0d", n, aa[7], aa.exists(8), aa.num());
    end
endmodule
