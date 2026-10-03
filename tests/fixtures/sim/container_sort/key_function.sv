// IEEE 1800-2009 7.12.2: the `with` key may be any expression of the receiver's
// element, including a function call; ties cannot occur with this permutation.
module tb;
    int q[$];
    int d[];

    function automatic int swapped_digits(int value);
        return (value % 10) * 10 + value / 10;
    endfunction

    initial begin
        for (int i = 0; i < 100; i++) q.push_back((i * 37) % 100);
        q.sort() with (swapped_digits(item));
        $write("q.sort");
        foreach (q[i]) $write(" %0d", q[i]);
        $display;

        d = new[100];
        for (int i = 0; i < 100; i++) d[i] = (i * 37) % 100;
        d.rsort() with (swapped_digits(item));
        $write("d.rsort");
        foreach (d[i]) $write(" %0d", d[i]);
        $display;
    end
endmodule
