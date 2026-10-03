// 20000-element sorts through a queue and a dynamic array. The sorted result is
// summarized by an order-sensitive checksum plus ordering checks; the test
// recomputes the same values from the generator below.
module tb;
    localparam int N = 20000;
    int q[$];
    int d[];
    longint unsigned state;
    longint unsigned checksum;
    int ordered;

    function automatic int unsigned next_value();
        state = state * 64'd6364136223846793005 + 64'd1442695040888963407;
        return int'(state >> 33) % 1000;
    endfunction

    initial begin
        state = 64'd12345;
        for (int i = 0; i < N; i++) q.push_back(next_value());

        q.sort();
        checksum = 0;
        ordered = 1;
        foreach (q[i]) begin
            checksum = checksum * 31 + q[i];
            if (i > 0 && q[i - 1] > q[i]) ordered = 0;
        end
        $display("q.sort ordered=%0d first=%0d last=%0d checksum=%0d",
                 ordered, q[0], q[N - 1], checksum);

        q.rsort() with (item);
        checksum = 0;
        ordered = 1;
        foreach (q[i]) begin
            checksum = checksum * 31 + q[i];
            if (i > 0 && q[i - 1] < q[i]) ordered = 0;
        end
        $display("q.rsort ordered=%0d first=%0d last=%0d checksum=%0d",
                 ordered, q[0], q[N - 1], checksum);

        // Key is the value modulo 100; equal keys keep the receiver's order,
        // so a second sort by the same key must not move anything.
        d = new[N];
        state = 64'd777;
        for (int i = 0; i < N; i++) d[i] = i * 1000 + int'(next_value() % 100);
        d.sort() with (item % 100);
        checksum = 0;
        ordered = 1;
        foreach (d[i]) begin
            checksum = checksum * 31 + d[i];
            if (i > 0 && d[i - 1] % 100 > d[i] % 100) ordered = 0;
            if (i > 0 && d[i - 1] % 100 == d[i] % 100 && d[i - 1] > d[i]) ordered = 0;
        end
        $display("d.sort_key ordered=%0d first=%0d last=%0d checksum=%0d",
                 ordered, d[0], d[N - 1], checksum);
    end
endmodule
