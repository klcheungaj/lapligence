// A record with a queue member as an element of a queue is a legal `ref`
// actual (SV 13.5.2); its formal carries the queue as a companion container,
// which an element does not have, so llg rejects it.
module tb;
    typedef struct { int a; int q[$]; } r_t;
    r_t list[$];
    r_t one;
    function automatic void f(ref r_t r);
        r.a = 1;
    endfunction
    initial begin
        list.push_back(one);
        f(list[0]);
        $display("%0d", list[0].a);
    end
endmodule
