   
     async function sendRequest(url, method, body) {
        let csrfToken = localStorage.getItem('csrfToken') || "";

     const headers = {
          'Content-Type': 'application/json',
       };
     if (csrfToken) {
            headers['X-CSRF-Token'] = csrfToken
        }
     console.log('sendRequest CSRF Token before fetch:', csrfToken);
 try {
       const response =  await fetch(url, {
             method: method,
               headers: headers,
              body: JSON.stringify(body)
           });
 
       if (!response.ok) {
                 try {
                     const errorData = await response.json();
                      throw new Error(errorData.message);
                   } catch (jsonError) {
                      throw new Error(`Network response was not ok: ${response.statusText}`);
                   }
               }
         const data = await response.json();
           console.log('Success data:', data);
           csrfToken = data.csrf_token;
          localStorage.setItem('csrfToken', csrfToken);
           return data;
    } catch (error) {
        console.error('Error:', error);
       throw error;
   }
}