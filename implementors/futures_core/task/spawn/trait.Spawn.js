(function() {var implementors = {};
implementors["futures_executor"] = [{text:"impl <a class=\"trait\" href=\"futures_core/task/spawn/trait.Spawn.html\" title=\"trait futures_core::task::spawn::Spawn\">Spawn</a> for <a class=\"struct\" href=\"futures_executor/struct.LocalSpawner.html\" title=\"struct futures_executor::LocalSpawner\">LocalSpawner</a>",synthetic:false,types:["futures_executor::local_pool::LocalSpawner"]},{text:"impl <a class=\"trait\" href=\"futures_core/task/spawn/trait.Spawn.html\" title=\"trait futures_core::task::spawn::Spawn\">Spawn</a> for <a class=\"struct\" href=\"futures_executor/struct.ThreadPool.html\" title=\"struct futures_executor::ThreadPool\">ThreadPool</a>",synthetic:false,types:["futures_executor::thread_pool::ThreadPool"]},{text:"impl&lt;'_&gt; <a class=\"trait\" href=\"futures_core/task/spawn/trait.Spawn.html\" title=\"trait futures_core::task::spawn::Spawn\">Spawn</a> for &amp;'_ <a class=\"struct\" href=\"futures_executor/struct.ThreadPool.html\" title=\"struct futures_executor::ThreadPool\">ThreadPool</a>",synthetic:false,types:["futures_executor::thread_pool::ThreadPool"]},];
implementors["futures_util"] = [{text:"impl&lt;'a&gt; <a class=\"trait\" href=\"futures_core/task/spawn/trait.Spawn.html\" title=\"trait futures_core::task::spawn::Spawn\">Spawn</a> for <a class=\"struct\" href=\"futures_util/stream/struct.FuturesUnordered.html\" title=\"struct futures_util::stream::FuturesUnordered\">FuturesUnordered</a>&lt;<a class=\"struct\" href=\"futures_core/future/future_obj/struct.FutureObj.html\" title=\"struct futures_core::future::future_obj::FutureObj\">FutureObj</a>&lt;'a, <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.unit.html\">()</a>&gt;&gt;",synthetic:false,types:["futures_util::stream::futures_unordered::FuturesUnordered"]},];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        
})()
