// IEEE 1800-2009 9.2.2.2.1 and 6.16: string variables, including string
// members of unpacked records, are ordinary storage for implicit
// sensitivity. A store that changes the string wakes its readers, an identical
// store does not, and a string the process writes itself is excluded from its
// always_comb sensitivity.
module tb;
    typedef struct { string s; int i; } t_t;
    t_t a;
    string name, tag;
    int n, star_len, own_len;
    int n_cnt = 0, star_cnt = 0;
    int bn, bs;

    always_comb begin
        n = a.s.len() + a.i;
        n_cnt = n_cnt + 1;
    end
    always @* begin
        star_len = name.len();
        star_cnt = star_cnt + 1;
    end
    always_comb begin
        tag = {name, "!"};
        own_len = tag.len();
    end

    task automatic show(input string label);
        $display("%s %0d %0d %0d %s +%0d +%0d", label, n, star_len, own_len, tag,
                 n_cnt - bn, star_cnt - bs);
    endtask

    initial begin
        a.s = "ab";
        a.i = 1;
        name = "xy";
        #1 bn = n_cnt;
        bs = star_cnt;
        show("t1");
        a.s = "abcd";
        #1 show("t2");
        a.s = "abcd";
        name = "xy";
        #1 show("t3");
        name = "xyz";
        #1 show("t4");
        $finish(0);
    end
endmodule
